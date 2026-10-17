# Run only for this interactive SSH session. The remote user's startup files are
# read normally; integration lives in a private temporary folder and is removed.
SNZ_SSH_TOKEN=$1
export SNZ_SSH_TOKEN
case ${SHELL##*/} in bash|zsh) ;; *)
  printf '\033]777;schnauz-ssh;unsupported;%s\007' "$SNZ_SSH_TOKEN"
  exec "${SHELL:-/bin/sh}" -il ;;
esac
umask 077
SNZ_SSH_INTEGRATION=$(mktemp -d "${TMPDIR:-/tmp}/schnauz-shell.XXXXXXXX") || exec "$SHELL" -il
export SNZ_SSH_INTEGRATION
trap 'rm -rf -- "$SNZ_SSH_INTEGRATION"' EXIT
cat > "$SNZ_SSH_INTEGRATION/hooks" <<'SNZ_HOOKS'
_schnauz_remote_prompt() {
  local snz_status=$?
  local snz_path
  snz_path=$(printf '%s' "$PWD" | od -An -tx1 | tr -d ' \n')
  printf '\033]777;schnauz-ssh;ready;%s;%s\007' "$SNZ_SSH_TOKEN" "$snz_path"
  return "$snz_status"
}
SNZ_HOOKS
case ${SHELL##*/} in
  bash)
    cat > "$SNZ_SSH_INTEGRATION/bashrc" <<'SNZ_BASH'
if [[ ${SNZ_SSH_TEST_SHELL:-0} != 1 ]]; then
  [[ -r /etc/profile ]] && source /etc/profile
  if [[ -r ~/.bash_profile ]]; then source ~/.bash_profile
  elif [[ -r ~/.bash_login ]]; then source ~/.bash_login
  elif [[ -r ~/.profile ]]; then source ~/.profile
  elif [[ -r ~/.bashrc ]]; then source ~/.bashrc
  fi
fi
[[ ${SNZ_SSH_TEST_SHELL:-0} == 1 ]] && HISTFILE=/dev/null
source "$SNZ_SSH_INTEGRATION/hooks"
# Keep existing prompt hooks, including array-valued PROMPT_COMMAND.
if [[ $(declare -p PROMPT_COMMAND 2>/dev/null) == 'declare -a '* ]]; then
  PROMPT_COMMAND+=(_schnauz_remote_prompt)
else
  PROMPT_COMMAND="_schnauz_remote_prompt${PROMPT_COMMAND:+; $PROMPT_COMMAND}"
fi
SNZ_BASH
    "$SHELL" --noprofile --rcfile "$SNZ_SSH_INTEGRATION/bashrc" -i
    ;;
  zsh)
    SNZ_SSH_USER_ZDOTDIR=${ZDOTDIR:-$HOME}; export SNZ_SSH_USER_ZDOTDIR
    cat > "$SNZ_SSH_INTEGRATION/.zshenv" <<'SNZ_ENV'
ZDOTDIR=$SNZ_SSH_USER_ZDOTDIR
[[ ${SNZ_SSH_TEST_SHELL:-0} != 1 && -r $ZDOTDIR/.zshenv ]] && source "$ZDOTDIR/.zshenv"
SNZ_SSH_USER_ZDOTDIR=${ZDOTDIR:-$HOME}
export SNZ_SSH_USER_ZDOTDIR
export ZDOTDIR=$SNZ_SSH_INTEGRATION
SNZ_ENV
    cat > "$SNZ_SSH_INTEGRATION/.zprofile" <<'SNZ_PROFILE'
ZDOTDIR=$SNZ_SSH_USER_ZDOTDIR
[[ ${SNZ_SSH_TEST_SHELL:-0} != 1 && -r $ZDOTDIR/.zprofile ]] && source "$ZDOTDIR/.zprofile"
SNZ_SSH_USER_ZDOTDIR=${ZDOTDIR:-$HOME}
export SNZ_SSH_USER_ZDOTDIR
export ZDOTDIR=$SNZ_SSH_INTEGRATION
SNZ_PROFILE
    cat > "$SNZ_SSH_INTEGRATION/.zshrc" <<'SNZ_ZSH'
ZDOTDIR=$SNZ_SSH_USER_ZDOTDIR
[[ ${SNZ_SSH_TEST_SHELL:-0} != 1 && -r $ZDOTDIR/.zshrc ]] && source "$ZDOTDIR/.zshrc"
[[ ${SNZ_SSH_TEST_SHELL:-0} == 1 ]] && HISTFILE=/dev/null
source "$SNZ_SSH_INTEGRATION/hooks"
autoload -Uz add-zsh-hook
add-zsh-hook precmd _schnauz_remote_prompt
SNZ_ZSH
    ZDOTDIR=$SNZ_SSH_INTEGRATION "$SHELL" -il
    ;;
esac
SNZ_STATUS=$?
exit "$SNZ_STATUS"
