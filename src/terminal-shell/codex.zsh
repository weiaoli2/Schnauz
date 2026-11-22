# Session-specific Codex lifecycle hooks. No user config/dotfiles are modified.
# Each command embeds its terminal channel, so daemon environment inheritance
# can never route a second terminal's time to the first terminal's project.
function schnauz-codex() {
  if [[ -n "$SCHNAUZ_CODEX_STATUS_CONFIG" ]]; then
    command codex -c "$SCHNAUZ_CODEX_STATUS_CONFIG" "$@"
  else
    command codex "$@"
  fi
}

# Retain the user's custom launchers. They can explicitly use schnauz-codex.
if (( ! ${+functions[codex]} && ! ${+aliases[codex]} )); then
  function codex() {
    if [[ -n "$SCHNAUZ_TIME_CHANNEL" && -f "$SCHNAUZ_TIME_CHANNEL/enabled" ]]; then
      schnauz-codex "$@"
    else
      command codex "$@"
    fi
  }
fi
