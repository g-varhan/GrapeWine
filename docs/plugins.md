# 🔌 GrapeWine Custom Addon & Plugin Guide

GrapeWine supports custom game indexers and metadata source databases through sandboxed Lua plugins. This guide explains how to write, configure, and install your own plugins to fetch games from any custom API, website, or local database list.

---

## 📁 Plugin Location

All Lua plugins must be placed in the `plugins/` directory at the root of your GrapeWine installation or workspace.
For installed installations, copy plugins to:
`~/.local/share/grapevine/plugins/`

GrapeWine loads all `.lua` files in this directory automatically on startup.

---

## ⚙️ Plugin Structure

Every plugin must return a Lua table containing:
1. `metadata`: Structure containing metadata like ID, Name, and permissions.
2. `search(query)`: A function that takes a query string and returns a list of matching game structures.
3. `list_installed()`: A function returning any currently installed games discovered by the plugin.

### Template Boilerplate (`custom_db.lua`)

Below is a complete, copy-pasteable boilerplate for writing a custom database plugin:

```lua
local plugin = {}

-- 1. Plugin Metadata
plugin.metadata = {
    id = "my_custom_db",
    name = "My Custom Game Store",
    permissions = { "read_fs" } -- List required permissions: 'read_fs', 'write_fs', 'execute'
}

-- 2. Search implementation
-- Returns an array of game structures.
function plugin.search(query)
    -- Your database can be a static list, or dynamically parsed using GrapeWine APIs
    local database = {
        {
            id = "custom_tux",
            title = "SuperTuxKart (Magnet Link)",
            source = "Custom DB",
            exec_path = "magnet:?xt=urn:btih:3b1a8d9a4b8c9d2e1a7b6c5d4e3f2a1b0c9d8e7f&dn=SuperTuxKart&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce",
            launch_args = {}
        },
        {
            id = "custom_openttd",
            title = "OpenTTD (Direct HTTP Download Link)",
            source = "Custom DB",
            exec_path = "https://proxy.openttd.org/openttd-releases/14.1/openttd-14.1-windows-win64.zip",
            launch_args = { "--fullscreen" }
        }
    }
    
    local results = {}
    local q = string.lower(query)
    for _, game in ipairs(database) do
        if string.find(string.lower(game.title), q) then
            -- Wrap the game in the schema format required by GrapeWine core
            table.insert(results, {
                id = game.id,
                title = game.title,
                source = game.source,
                exec_path = game.exec_path, -- Magnet or HTTP direct download URL
                launch_args = game.launch_args or {},
                cover_art = nil -- Optional URL string
            })
        end
    end
    return results
end

-- 3. Discover already installed games
function plugin.list_installed()
    return {}
end

return plugin
```

---

## 🔒 The Sandboxed GrapeWine API

To keep your system secure, GrapeWine executes Lua plugins inside a sandboxed environment where standard unsafe modules (such as `io`, `os.execute`, and standard `require`) are stripped.

Instead, the host injects the safe, controlled `grapevine` namespace:

### 1. Networking
* **`grapevine.http_get(url)`**
  Performs a synchronous HTTP GET request and returns the raw response body string.
  * *Example*:
    ```lua
    local response = grapevine.http_get("https://api.github.com/repos/user/repo/releases")
    ```

### 2. Cache Registry DB
* **`grapevine.write_cache(key, value)`**
  Saves a persistent string value into GrapeWine's global JSON cache.
  * *Example*:
    ```lua
    grapevine.write_cache("my_api_key", "abcdef123456")
    ```
* **`grapevine.read_cache(key)`**
  Retrieves a saved string value from the cache. Returns `nil` if not found.
  * *Example*:
    ```lua
    local key = grapevine.read_cache("my_api_key")
    ```

### 3. File System Access
* **`grapevine.read_file(absolute_path)`**
  Reads the full contents of a file on the host filesystem.
  * *Example*:
    ```lua
    local content = grapevine.read_file("/home/user/.config/game/config.ini")
    ```

### 4. Process Shell Execution
* **`grapevine.execute(command, args_table)`**
  Executes a localized shell command (e.g. `ls`, `grep`) and returns stdout.
  * *Example*:
    ```lua
    local output = grapevine.execute("ls", { "-la", "/home/user" })
    ```

---

## 💡 Best Practices

1. **Intelligent IDs**: Prefix your game IDs with the plugin name (e.g., `steam_1230`, `custom_tux`) to prevent collisions with other store integrations.
2. **Setup Detection**: Direct HTTP download links must point to a standard archive (`.zip`, `.tar.gz`) or a silent executable installer (`setup.exe`). GrapeWine will automatically extract archives, run installer executables silently in Wine, scan the prefix drive C: for the largest EXE, and configure it to boot automatically.
