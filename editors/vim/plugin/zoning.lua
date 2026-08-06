-- `.zone` is shared with BIND, so claim only the files that lead with the
-- declaration a contract leads with. Returning nil hands the rest back to
-- Neovim's own detection instead of guessing at them; see ftdetect/zoning.vim,
-- which this mirrors for Vim.
vim.filetype.add({
  extension = {
    zone = function(_, bufnr)
      for _, line in ipairs(vim.api.nvim_buf_get_lines(bufnr, 0, 64, false)) do
        if not line:match("^%s*$") and not line:match("^%s*//") then
          local opener = line:match("^%s*(%a+)%f[%W]")
          return (opener == "package" or opener == "workspace") and "zoning" or nil
        end
      end
      return "zoning"
    end,
  },
})

if vim.fn.has("nvim-0.11") == 1 then
  vim.lsp.enable("zoning")
end

local ok, devicons = pcall(require, "nvim-web-devicons")
if ok then
  local icon = vim.g.zoning_ascii_icon == 1 and "[=]"
    or vim.g.zoning_nerd_font == 1 and (vim.g.zoning_nerd_font_icon or "󰙅")
    or "≡"
  devicons.set_icon({
    zone = { icon = icon, color = "#718096", name = "Zoning" },
  })
end
