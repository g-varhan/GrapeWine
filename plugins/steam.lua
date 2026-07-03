local plugin = {}

plugin.metadata = {
    id = "steam",
    name = "Steam Integration",
    permissions = { "read_fs", "execute" }
}

function plugin.search(query)
    local games = {
        {
            id = "steam_cyberpunk",
            title = "Cyberpunk 2077",
            source = "Steam",
            exec_path = "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Cyberpunk 2077\\bin\\x64\\Cyberpunk2077.exe",
            launch_args = {},
            cover_art = nil
        },
        {
            id = "steam_witcher3",
            title = "The Witcher 3: Wild Hunt",
            source = "Steam",
            exec_path = "C:\\Program Files (x86)\\Steam\\steamapps\\common\\The Witcher 3\\bin\\x64\\witcher3.exe",
            launch_args = {},
            cover_art = nil
        },
        {
            id = "steam_halflife",
            title = "Half-Life 2",
            source = "Steam",
            exec_path = "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Half-Life 2\\hl2.exe",
            launch_args = { "-game", "hl2" },
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
    return {
        {
            id = "steam_halflife",
            title = "Half-Life 2",
            source = "Steam",
            exec_path = "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Half-Life 2\\hl2.exe",
            launch_args = { "-game", "hl2" },
            cover_art = nil
        }
    }
end

return plugin
