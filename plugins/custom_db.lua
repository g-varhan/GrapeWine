local plugin = {}

plugin.metadata = {
    id = "custom_db",
    name = "My Custom Game Database",
    permissions = { "read_fs" }
}

-- Returns a list of game candidates matching the search query.
--
-- Each game item must follow this structure:
--   - id: Unique string identifier (e.g. "my_custom_game_1")
--   - title: Human-readable game title
--   - source: Name of the database source
--   - exec_path: Magnet link, torrent URL, or direct HTTP/HTTPS link to zip file or setup installer
--   - launch_args: Optional table of launch flags
--   - cover_art: Optional URL to image
function plugin.search(query)
    -- Database of custom game mappings
    local database = {
        {
            id = "custom_game_openttd_direct",
            title = "OpenTTD (Direct HTTP Download Demo)",
            source = "Custom DB",
            -- Direct download path to a zip file
            exec_path = "https://proxy.openttd.org/openttd-releases/14.1/openttd-14.1-windows-win64.zip",
        },
        {
            id = "custom_game_supertuxkart_torrent",
            title = "SuperTuxKart (Magnet Torrent Demo)",
            source = "Custom DB",
            -- Torrent magnet link
            exec_path = "magnet:?xt=urn:btih:3b1a8d9a4b8c9d2e1a7b6c5d4e3f2a1b0c9d8e7f&dn=SuperTuxKart&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce",
        }
    }
    
    local results = {}
    local q = string.lower(query)
    for _, g in ipairs(database) do
        if string.find(string.lower(g.title), q) then
            table.insert(results, {
                id = g.id,
                title = g.title,
                source = g.source,
                exec_path = g.exec_path,
                launch_args = {},
                cover_art = nil
            })
        end
    end
    return results
end

function plugin.list_installed()
    return {}
end

return plugin
