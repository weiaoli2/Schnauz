//! Follow symlinks using SFTP STAT, without reading file contents or running a
//! remote shell. The authenticated listing transport must already exist.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    Folder,
    File,
    Unavailable,
}

pub(super) fn resolve(
    server: &SftpServer,
    directory: &str,
    entries: &mut [Entry],
    use_system: bool,
    cancel: Arc<AtomicBool>,
) {
    let links: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, entry)| entry.link.map(|_| i))
        .collect();
    if links.is_empty() {
        return;
    }
    let server = ssh_connection::for_reconnect(server, use_system);
    for batch in links.chunks(128) {
        if cancel.load(Ordering::Acquire) {
            return;
        }
        // INIT v3 followed by read-only STAT requests after version negotiation.
        let mut input = Vec::new();
        packet(&mut input, &[1, 0, 0, 0, 3]);
        for (id, &index) in batch.iter().enumerate() {
            let path = Path::new(directory).join(&entries[index].name);
            let path = path.as_os_str().as_encoded_bytes();
            let mut request = vec![17]; // SSH_FXP_STAT follows the entire link chain.
            request.extend_from_slice(&(id as u32).to_be_bytes());
            request.extend_from_slice(&(path.len() as u32).to_be_bytes());
            request.extend_from_slice(path);
            packet(&mut input, &request);
        }
        let mut command = Command::new("/usr/bin/ssh");
        // Fail closed if the listing's mux vanishes: no reconnect/auth fallback,
        // no shell, PTY, prompt hooks or input to the user's terminal.
        command.args([
            "-T",
            "-o",
            "ControlMaster=no",
            "-o",
            "BatchMode=yes",
            "-o",
            "ProxyCommand=false",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "RemoteCommand=none",
            "-o",
            "ClearAllForwardings=yes",
            "-o",
            "PermitLocalCommand=no",
            "-o",
        ]);
        command.arg(format!(
            "ControlPath={}",
            ssh_control_path(&server).display()
        ));
        if server.port != 0 {
            command.arg("-p").arg(server.port.to_string());
        }
        command
            .arg("-s")
            .arg(server.connection_target())
            .arg("sftp");
        let count = batch.len();
        let result = workers::command_exchange(
            command,
            Duration::from_secs(20),
            cancel.clone(),
            Box::new(move |mut stdin, mut stdout| {
                use std::io::Write;
                stdin.write_all(&input[..9])?;
                let mut response = read_packet(&mut stdout)?;
                if response.get(4..9) != Some(&[2, 0, 0, 0, 3][..]) {
                    return Err(std::io::Error::other("Unsupported SFTP metadata version."));
                }
                stdin.write_all(&input[9..])?;
                for _ in 0..count {
                    response.extend(read_packet(&mut stdout)?);
                }
                // Keep stdin open until every reply has arrived. OpenSSH exits
                // at EOF without flushing pending replies.
                Ok(response)
            }),
        );
        let Ok(output) = result else {
            return;
        };
        if !output.status.success() {
            return;
        }
        let Some(targets) = replies(&output.stdout, batch.len()) else {
            return;
        };
        for (&index, target) in batch.iter().zip(targets) {
            if let Some((target, size)) = target {
                entries[index].link = Some(target);
                entries[index].is_dir = target == Target::Folder;
                if let Some(size) = size {
                    entries[index].size = size;
                }
            }
        }
    }
}

fn packet(output: &mut Vec<u8>, payload: &[u8]) {
    output.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    output.extend_from_slice(payload);
}

fn read_packet(reader: &mut impl std::io::Read) -> std::io::Result<Vec<u8>> {
    let mut header = [0; 4];
    reader.read_exact(&mut header)?;
    let size = u32::from_be_bytes(header) as usize;
    if size == 0 || size > 256 * 1024 {
        return Err(std::io::Error::other("Invalid SFTP metadata packet size."));
    }
    let mut packet = vec![0; size + 4];
    packet[..4].copy_from_slice(&header);
    reader.read_exact(&mut packet[4..])?;
    Ok(packet)
}

fn u32_field(bytes: &mut &[u8]) -> Option<u32> {
    let value = u32::from_be_bytes(bytes.get(..4)?.try_into().ok()?);
    *bytes = &bytes[4..];
    Some(value)
}

fn next_packet<'a>(bytes: &mut &'a [u8]) -> Option<&'a [u8]> {
    let length = u32_field(bytes)? as usize;
    if length == 0 || length > 256 * 1024 {
        return None;
    }
    let packet = bytes.get(..length)?;
    *bytes = &bytes[length..];
    Some(packet)
}

fn replies(mut bytes: &[u8], count: usize) -> Option<Vec<Option<(Target, Option<u64>)>>> {
    let version = next_packet(&mut bytes)?;
    if version.first() != Some(&2) || version.get(1..5)? != 3u32.to_be_bytes() {
        return None;
    }
    let mut targets = vec![None; count];
    let mut seen = vec![false; count];
    for _ in 0..count {
        let packet = next_packet(&mut bytes)?;
        let kind = *packet.first()?;
        let mut fields = &packet[1..];
        let id = u32_field(&mut fields)? as usize;
        if id >= count || seen[id] {
            return None;
        }
        seen[id] = true;
        match kind {
            101 => {
                // STATUS: missing, inaccessible, looping or unsupported target.
                let _status = u32_field(&mut fields)?;
            }
            105 => {
                // ATTRS, in the v3 wire order.
                let flags = u32_field(&mut fields)?;
                let size = if flags & 1 != 0 {
                    let size = u64::from_be_bytes(fields.get(..8)?.try_into().ok()?);
                    fields = &fields[8..];
                    Some(size)
                } else {
                    None
                };
                if flags & 2 != 0 {
                    fields = fields.get(8..)?;
                }
                if flags & 4 != 0 {
                    let mode = u32_field(&mut fields)?;
                    let target = match mode & 0o170000 {
                        0o040000 => Target::Folder,
                        0o100000 => Target::File,
                        _ => Target::Unavailable,
                    };
                    targets[id] = Some((target, size));
                }
            }
            _ => return None,
        }
    }
    bytes.is_empty().then_some(targets)
}
