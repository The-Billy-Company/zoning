-- Copyright (c) 2026 The Billy Company
return {
  cmd = { vim.g.zoning_executable or "zoning", "lsp", "--stdio" },
  filetypes = { "zoning" },
  root_markers = { "contract", ".git" },
}
