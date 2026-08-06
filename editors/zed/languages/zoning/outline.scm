(package_declaration
  name: (word) @name) @item

(workspace_setting
  "member"
  value: (paths
    (word) @name)) @item

(zone_definition
  name: (word) @name) @item

(seal_declaration
  subject: (word) @name) @item

(keep_declaration
  subject: (word) @name) @item

(use_declaration
  modules: (paths
    (word) @name)) @item

(variance_declaration
  law: (law) @name) @item
