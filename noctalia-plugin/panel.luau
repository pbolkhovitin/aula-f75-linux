--!nonstrict
-- Detail panel for the AULA F75 widget. Read-only view plus two actions:
-- refresh the snapshot and open the desktop app.
--
-- The panel deliberately shows what a person can act on (connection, profile,
-- lighting, remaps) and hides protocol details such as the raw access code,
-- firmware number or hidraw paths; those live in the app's Log page.

local snapshot = noctalia.state.get("f75_status")
local busy = false
local lastError = nil

local function config(key, fallback)
  local value = noctalia.getConfig(key)
  if value == nil then
    return fallback
  end
  return value
end

local function command()
  local value = config("command", "f75-control")
  if type(value) ~= "string" or value == "" then
    return "f75-control"
  end
  return value
end

local function row(icon, label, value, color)
  return ui.row({ gap = 8, align = "center", flexGrow = 1 }, {
    ui.glyph({ name = icon, size = 14, color = color or "on_surface_variant" }),
    ui.column({ gap = 1, flexGrow = 1 }, {
      ui.label({ text = label, fontSize = 11, color = "on_surface_variant" }),
      ui.label({ text = value, fontWeight = "medium", color = color or "on_surface" }),
    }),
  })
end

local function accessColor(device)
  if device.access == "denied" or device.access == "error" then
    return "error"
  end
  return device.write_supported and "primary" or "secondary"
end

local function statusColor(data)
  if data == nil then
    return "on_surface_variant"
  end
  local device = data.device or {}
  if not device.present then
    return "on_surface_variant"
  end
  return accessColor(device)
end

local function accessText(device)
  if device.access == "denied" then
    return noctalia.tr("panel.access_denied")
  end
  if device.access == "error" then
    return noctalia.tr("panel.access_error")
  end
  return noctalia.tr("panel.access_ok")
end

local function render()
  local data = snapshot
  local device = data and data.device or {}
  local state = data and data.state or {}
  local lighting = state.lighting or {}
  local profile = state.profile or {}
  local prefs = state.prefs or {}
  local macros = state.macros or {}
  local keys = state.keys or {}
  local color = statusColor(data)

  local body = {}
  table.insert(body, ui.row({ gap = 9, align = "center" }, {
    ui.glyph({ name = device.present and "keyboard" or "keyboard-off", size = 18, color = color }),
    ui.column({ gap = 1, flexGrow = 1 }, {
      ui.label({ text = "AULA F75", fontSize = 16, fontWeight = "bold" }),
      ui.label({
        text = data and tostring(device.transport_label or device.transport) or noctalia.tr("panel.waiting"),
        fontSize = 11,
        color = color,
      }),
    }),
  }))

  if data == nil then
    table.insert(body, ui.label({ text = noctalia.tr("panel.waiting"), color = "on_surface_variant" }))
  elseif not device.present then
    table.insert(
      body,
      ui.column({ gap = 3, padding = 10, radius = 10, fill = "surface/0.48" }, {
        ui.label({ text = noctalia.tr("widget.not_connected") }),
        ui.label({ text = noctalia.tr("panel.connect_hint"), fontSize = 10, color = "on_surface_variant" }),
      })
    )
  else
    if not device.write_supported then
      table.insert(
        body,
        ui.column({ gap = 3, padding = 10, radius = 10, fill = "secondary/0.10" }, {
          ui.label({ text = accessText(device), fontWeight = "medium", color = color }),
          ui.label({ text = noctalia.tr("panel.writes_limited_hint"), fontSize = 10, color = "on_surface_variant", maxLines = 3 }),
        })
      )
    end
    table.insert(
      body,
      ui.column({ gap = 3, padding = 10, radius = 10, fill = "surface/0.48" }, {
        row("user", noctalia.tr("panel.profile"), tostring(profile.name or "—")),
        row("sparkles", noctalia.tr("panel.effect"), tostring(lighting.effect_name or "—")),
        row("sun", noctalia.tr("panel.brightness"), string.format("%d%%", tonumber(lighting.brightness_percent) or 0)),
        row("palette", noctalia.tr("panel.color"), tostring(lighting.color or "—")),
      })
    )
    table.insert(
      body,
      ui.column({ gap = 3, padding = 10, radius = 10, fill = "surface/0.48" }, {
        row("adjustments", noctalia.tr("panel.keys"), noctalia.tr("panel.keys_value", {
          remapped = tonumber(keys.remapped) or 0,
          fn = tonumber(keys.remapped_fn) or 0,
        })),
        row("list", noctalia.tr("panel.macros"), noctalia.tr("panel.macros_value", {
          count = tonumber(macros.count) or 0,
          steps = tonumber(macros.steps) or 0,
        })),
        row("bolt", noctalia.tr("panel.power"), noctalia.tr("panel.power_value", {
          debounce = tonumber(prefs.debounce_ms) or 0,
          sleep = tonumber(prefs.sleep_seconds) or 0,
        })),
      })
    )
  end

  local warnings = data and data.warnings or {}
  if #warnings > 0 then
    local warningRows = {}
    for index = 1, math.min(#warnings, 2) do
      table.insert(warningRows, ui.label({
        text = "• " .. tostring(warnings[index]),
        fontSize = 10,
        color = "secondary",
        maxLines = 3,
      }))
    end
    table.insert(
      body,
      ui.column({ gap = 4, padding = 10, radius = 10, fill = "secondary/0.10" }, warningRows)
    )
  end

  if lastError ~= nil then
    table.insert(body, ui.label({ text = "⚠ " .. tostring(lastError), fontSize = 10, color = "error", maxLines = 3 }))
  end

  table.insert(body, ui.row({ gap = 8, align = "center" }, {
    ui.button({ text = noctalia.tr("panel.refresh"), glyph = "refresh", variant = "outline", onClick = onRefreshClicked }),
    ui.button({ text = noctalia.tr("panel.open_app"), glyph = "external-link", variant = "primary", onClick = onOpenAppClicked }),
  }))

  panel.render(ui.column({ gap = 11, align = "stretch" }, body))
end

local function fetch()
  if busy then
    return
  end
  busy = true
  local started = noctalia.runAsync({ command(), "--status" }, function(result)
    busy = false
    if result.exitCode ~= 0 or result.timedOut then
      lastError = result.timedOut and "timeout" or (result.stderr ~= "" and result.stderr or ("exit " .. tostring(result.exitCode)))
      render()
      return
    end
    local data = noctalia.json.decode(result.stdout)
    if type(data) ~= "table" then
      lastError = "invalid JSON"
      render()
      return
    end
    lastError = nil
    snapshot = data
    noctalia.state.set("f75_status", data)
    render()
  end, 5000)
  if not started then
    busy = false
    lastError = noctalia.tr("widget.binary_missing", { command = command() })
    render()
  end
end

noctalia.state.watch("f75_status", function(value)
  if type(value) == "table" then
    snapshot = value
    render()
  end
end)

function onOpen(_context)
  render()
  fetch()
end

function onClose() end

function onRefreshClicked()
  fetch()
end

function onOpenAppClicked()
  noctalia.runAsync({ command() })
end

render()
