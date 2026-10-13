# Platter Aurora verification

The screenshot uses fixture data rendered by the actual Ratatui widgets at 120×40. Run `AURORA_RENDER_DIR=/tmp/platter-renders cargo test --locked` to export SVG and text frames.

The render matrix covers 120×40, 100×30, 80×24, 60×20, 30×10, 20×8, and 1×1, plus shrinking and growing one terminal. Tests cover offline/empty screens and modal input precedence. Scan execution uses a mock Nmap binary; disk and Metasploit actions are not executed by rendering tests.

Compatibility: config/data paths retain the previous application directory; environment variables and destructive-operation confirmations retain their original behavior. No old command aliases are installed.
