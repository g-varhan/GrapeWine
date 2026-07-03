local plugin = {}

plugin.metadata = {
    id = "epic",
    name = "Epic Games Store",
    permissions = { "read_fs" }
}

function plugin.search(query)
    local games = {
        {
            id = "epic_gtav",
            title = "Grand Theft Auto V",
            source = "Epic",
            exec_path = "C:\\Program Files\\Epic Games\\GTAV\\PlayGTAV.exe",
            launch_args = {},
            cover_art = nil
        },
        {
            id = "epic_control",
            title = "Control",
            source = "Epic",
            exec_path = "C:\\Program Files\\Epic Games\\Control\\Control.exe",
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
