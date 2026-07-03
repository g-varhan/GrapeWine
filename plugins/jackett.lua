local plugin = {}

plugin.metadata = {
    id = "jackett",
    name = "Jackett / Prowlarr Indexer",
    permissions = { "read_fs" }
}

-- XML Torznab parser using Lua patterns
local function parse_torznab_xml(xml_body)
    local items = {}
    for item_str in string.gmatch(xml_body, "<item>(.-)</item>") do
        local title = string.match(item_str, "<title>(.-)</title>") or "Unknown Torrent"
        
        -- Extract magnet link (from enclosure, guid, link or custom torznab attributes)
        local magnet = string.match(item_str, 'value="(magnet:[^"]+)"') or 
                       string.match(item_str, "<guid[^>]*>(magnet:[^<]+)</guid>") or
                       string.match(item_str, "<link>(magnet:[^<]+)</link>")
                       
        if not magnet then
            -- Try to look for a magnet parameter in the link
            local link = string.match(item_str, "<link>(.-)</link>")
            if link and string.find(link, "magnet:") then
                magnet = link
            end
        end

        if magnet then
            title = string.gsub(title, "<!%[CDATA%[(.-)%]%]>", "%1")
            magnet = string.gsub(magnet, "<!%[CDATA%[(.-)%]%]>", "%1")
            
            -- Generate hash-based ID
            local hash = string.match(magnet, "btih:([%a%d]+)") or tostring(math.random(100000, 999999))
            
            table.insert(items, {
                id = "jackett_" .. hash,
                title = title .. " (Torrent)",
                source = "Jackett",
                exec_path = magnet, -- The magnet link serves as the executable path (download source)
                launch_args = {},
                cover_art = nil
            })
        end
    end
    return items
end

function plugin.search(query)
    -- Check config, fallback to default local Jackett Torznab URL
    local host = grapevine.read_cache("jackett_url") or "http://localhost:9117"
    local apikey = grapevine.read_cache("jackett_api_key") or "YOUR_API_KEY_HERE"
    
    if apikey == "YOUR_API_KEY_HERE" or apikey == "" then
        -- Return empty result but cache key so the user knows they can configure it
        grapevine.write_cache("jackett_api_key", "YOUR_API_KEY_HERE")
        grapevine.write_cache("jackett_url", "http://localhost:9117")
        return {}
    end

    -- Construct Torznab URL
    -- Category 4000 = PC Games
    local url = host .. "/api/v2.0/indexers/all/results/torznab/api?apikey=" .. apikey .. "&q=" .. query .. "&cat=4000"
    
    local xml_response = grapevine.http_get(url)
    if not xml_response or xml_response == "" then
        return {}
    end

    return parse_torznab_xml(xml_response)
end

function plugin.list_installed()
    return {}
end

return plugin
