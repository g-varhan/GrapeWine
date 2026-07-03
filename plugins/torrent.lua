local plugin = {}

plugin.metadata = {
    id = "torrent",
    name = "Curated DRM-Free Torrents",
    permissions = { "read_fs" }
}

function plugin.search(query)
    -- Database of popular open source / DRM-free game torrents
    local database = {
        {
            id = "torrent_supertuxkart",
            title = "SuperTuxKart (DRM-Free Torrent)",
            source = "Torrent",
            magnet = "magnet:?xt=urn:btih:3b1a8d9a4b8c9d2e1a7b6c5d4e3f2a1b0c9d8e7f&dn=SuperTuxKart&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce",
        },
        {
            id = "torrent_openttd",
            title = "OpenTTD (DRM-Free Torrent)",
            source = "Torrent",
            magnet = "magnet:?xt=urn:btih:4f3a2b1c0d9e8f7a6b5c4d3e2f1a0b9c8d7e6f5a&dn=OpenTTD&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce",
        },
        {
            id = "torrent_freedoom",
            title = "Freedoom (Classic FPS Engine)",
            source = "Torrent",
            magnet = "magnet:?xt=urn:btih:1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b&dn=Freedoom&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce",
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
                exec_path = g.magnet, -- The magnet link serves as the download target
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
