vim.filetype.add({
  extension = { zone = "zoning" },
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
