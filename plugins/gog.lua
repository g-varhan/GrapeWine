local plugin = {}

plugin.metadata = {
    id = "gog",
    name = "GOG.com Integration",
    permissions = { "read_fs" }
}

function plugin.search(query)
    local games = {
        {
            id = "gog_cyberpunk",
            title = "Cyberpunk 2077 (GOG Edition)",
            source = "GOG",
            exec_path = "C:\\GOG Games\\Cyberpunk 2077\\bin\\x64\\Cyberpunk2077.exe",
            launch_args = {},
            cover_art = nil
        },
        {
            id = "gog_hollowknight",
            title = "Hollow Knight",
            source = "GOG",
            exec_path = "C:\\GOG Games\\Hollow Knight\\hollow_knight.exe",
            launch_args = {},
            cover_art = nil
        }
    }
    
    local results = {}
    local q = string.lower(query)
    for _, g in ipairs(games) do
        if string.find(string.lower(g.title), q) then
            table.insert(results, g)
        end
    end
    return results
end

function plugin.list_installed()
    return {}
end

return plugin
