# Integrated interactive SSH. Commands, tunnels and custom RemoteCommand sessions
# retain normal OpenSSH behavior. No credentials or remote dotfiles are stored.
function _schnauz_ssh() {
  # The conservative parser identifies a plain interactive login only.
  local arg flags ch skip=0 dest='' unsupported=0
  for arg in "$@"; do
    if (( skip )); then skip=0; continue; fi
    if [[ -n "$dest" ]]; then unsupported=1; break; fi
    if [[ "$arg" == -- ]]; then continue; fi
    if [[ "$arg" != -* ]]; then dest="$arg"; continue; fi
    flags="${arg#-}"
    while [[ -n "$flags" ]]; do
      ch="${flags[1]}"; flags="${flags[2,-1]}"
      case "$ch" in
        N|f|n|T|s|V|G|W|O|Q|S) unsupported=1; break ;;
        B|b|c|D|E|e|F|I|i|J|L|l|m|o|P|p|R|w)
          [[ -z "$flags" ]] && skip=1
          flags='' ;;
        4|6|A|a|C|g|K|k|q|t|v|X|x|Y|y) ;;
        *) unsupported=1; break ;;
      esac
    done
    (( unsupported )) && break
  done
  if (( unsupported || skip || ZSH_SUBSHELL )) || [[ -z "$dest" || ! -t 0 || ! -t 1 ]]; then
    command ssh "$@"; return $?
  fi
  local config line host='' user='' port='' remote_command='none' session_type='default' fork_after='no' stdin_null='no' request_tty='auto'
  config=$(command ssh -G "$@" 2>/dev/null) || { command ssh "$@"; return $?; }
  for line in ${(f)config}; do
    case "$line" in
      'hostname '*) host="${line#hostname }" ;;
      'user '*) user="${line#user }" ;;
      'port '*) port="${line#port }" ;;
      'remotecommand '*) remote_command="${line#remotecommand }" ;;
      'sessiontype '*) session_type="${line#sessiontype }" ;;
      'forkafterauthentication '*) fork_after="${line#forkafterauthentication }" ;;
      'stdinnull '*) stdin_null="${line#stdinnull }" ;;
      'requesttty '*) request_tty="${line#requesttty }" ;;
    esac
  done
  if [[ -z "$host" || -z "$user" || -z "$port" || "$remote_command" != none || "$session_type" == none || "$session_type" == subsystem || "$fork_after" == yes || "$stdin_null" == yes || "$request_tty" == false || "$request_tty" == no ]]; then
    command ssh "$@"; return $?
  fi
  local socket_dir token socket bootstrap rc
  socket_dir=$(command mktemp -d /tmp/schnauz-ssh.XXXXXXXX) || { command ssh "$@"; return $?; }
  token="${socket_dir:t}"
  socket="$socket_dir/mux"
  # Metadata is read from our private local file, never accepted from terminal
  # output. The stream carries just an identifier that must match this file.
  (
    umask 077
    printf '%s\0' "$token" "$socket" "$host" "$user" "$port" "$PWD" > "${SHUFFLE_STUDIO_SSH_REQUEST}.new"
    command mv -f -- "${SHUFFLE_STUDIO_SSH_REQUEST}.new" "$SHUFFLE_STUDIO_SSH_REQUEST"
  ) || { command rmdir -- "$socket_dir"; command ssh "$@"; return $?; }
  bootstrap=$(<"$SHUFFLE_STUDIO_INTEGRATION_DIR/ssh-bootstrap.sh")
  # Quote the fixed bootstrap and generated token as shell arguments. User paths
  # and SSH arguments are not interpolated into remote executable text.
  # OpenSSH expands percent tokens in RemoteCommand before sending it.
  bootstrap="${bootstrap//\%/%%}"
  local remote="sh -c ${(qq)bootstrap} sh ${(qq)token}"
  printf '\e]777;schnauz-ssh;start;%s\a' "$token"
  rc=130
  {
    command ssh -tt -o "ControlPath=$socket" -o ControlMaster=auto -o ControlPersist=no -o "RemoteCommand=$remote" "$@"
    rc=$?
  } always {
    printf '\e]777;schnauz-ssh;end;%s\a' "$token"
    command rm -f -- "$SHUFFLE_STUDIO_SSH_REQUEST"
    # OpenSSH owns its socket. Never recursively remove a path returned by ssh.
    command rm -f -- "$socket"
    command rmdir -- "$socket_dir" 2>/dev/null
  }
  return $rc
}
# Respect an existing user-defined SSH function. `command ssh` remains an explicit
# escape hatch for ordinary SSH without browser integration.
if (( ! $+functions[ssh] )); then
  function ssh { _schnauz_ssh "$@"; }
fi
