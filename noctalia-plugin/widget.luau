--!nonstrict
-- Bar widget for the AULA F75 keyboard.
--
-- The widget is a thin consumer: `f75-control --status` (the desktop app in
-- another process) owns discovery, the settings file and the protocol. This
-- script never talks to hidraw itself, so it cannot drift from the app.

local snapshot = nil
local lastError = nil
local lastRun = 0
local busy = false
local launchFailed = false

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

local function refreshSeconds()
  local value = tonumber(config("refresh_seconds", 5)) or 5
  return math.max(2, math.min(120, value))
end

local function statusColor(data)
  if not config("colorize", true) then
    return "on_surface"
  end
  if data == nil then
    return "on_surface_variant"
  end
  local device = data.device or {}
  if device.access == "denied" or device.access == "error" then
    return "error"
  end
  if not device.present then
    return "on_surface_variant"
  end
  if not device.write_supported then
    return "secondary"
  end
  return "primary"
end

local function shortText(data)
  local device = data.device or {}
  local state = data.state or {}
  local lighting = state.lighting or {}
  local profile = state.profile or {}
  local mode = config("display", "full")

  if mode == "glyph" then
    return nil
  end

  local parts = {}
  if mode == "full" then
    table.insert(parts, "F75")
    if config("show_connection", false) then
      table.insert(
        parts,
        device.transport == "usb" and noctalia.tr("widget.usb") or noctalia.tr("widget.wireless")
      )
    end
  end
  if config("show_profile", true) and profile.name ~= nil then
    table.insert(parts, tostring(profile.name))
  end
  if config("show_effect", true) and lighting.effect_name ~= nil then
    local effect = tostring(lighting.effect_name)
    if mode == "compact" then
      effect = string.format("%s %d%%", effect, tonumber(lighting.brightness_percent) or 0)
    end
    table.insert(parts, effect)
  end
  if #parts == 0 then
    table.insert(parts, "F75")
  end
  return table.concat(parts, " · ")
end

local function tooltip(data)
  if data == nil then
    if launchFailed then
      return noctalia.tr("widget.binary_missing", { command = command() })
    end
    return noctalia.tr("widget.loading")
  end
  local device = data.device or {}
  local state = data.state or {}
  local lighting = state.lighting or {}
  local profile = state.profile or {}
  local prefs = state.prefs or {}
  local macros = state.macros or {}
  local keys = state.keys or {}
  local lines = {}

  if not device.present then
    table.insert(lines, noctalia.tr("widget.not_connected"))
  else
    table.insert(lines, "F75 · " .. tostring(device.transport_label or device.transport))
    table.insert(lines, noctalia.tr("widget.access", { value = tostring(device.access) }))
    if device.write_supported then
      table.insert(lines, noctalia.tr("widget.write_ok"))
    else
      table.insert(lines, noctalia.tr("widget.write_limited"))
    end
    table.insert(
      lines,
      noctalia.tr("widget.profile", { name = tostring(profile.name or "?"), index = tostring(profile.index or 0) })
    )
    table.insert(
      lines,
      noctalia.tr("widget.effect", {
        name = tostring(lighting.effect_name or "?"),
        brightness = tostring(lighting.brightness_percent or 0),
        speed = tostring(lighting.speed or 0),
      })
    )
    table.insert(lines, noctalia.tr("widget.color", { value = tostring(lighting.color or "?") }))
    table.insert(
      lines,
      noctalia.tr("widget.keys", {
        remapped = tostring(keys.remapped or 0),
        fn = tostring(keys.remapped_fn or 0),
        count = tostring(keys.count or 0),
      })
    )
    table.insert(
      lines,
      noctalia.tr("widget.macros", { count = tostring(macros.count or 0), steps = tostring(macros.steps or 0) })
    )
    table.insert(
      lines,
      noctalia.tr("widget.power", {
        debounce = tostring(prefs.debounce_ms or "?"),
        sleep = tostring(prefs.sleep_seconds or "?"),
      })
    )
  end

  local warnings = data.warnings or {}
  for index = 1, math.min(#warnings, 2) do
    table.insert(lines, "• " .. tostring(warnings[index]))
  end
  return table.concat(lines, "\n")
end

local function render()
  local device = snapshot and snapshot.device or nil
  local color = statusColor(snapshot)

  if snapshot ~= nil and not (device and device.present) and config("hide_when_disconnected", false) then
    barWidget.setVisible(false)
    return
  end
  barWidget.setVisible(true)

  local glyph = "keyboard"
  if snapshot == nil then
    glyph = launchFailed and "alert-circle" or "keyboard"
  elseif not (device and device.present) then
    glyph = "keyboard-off"
  elseif device.access == "denied" or device.access == "error" then
    glyph = "keyboard-off"
  end

  local children = {
    ui.glyph({ name = glyph, size = 15, color = color }),
  }
  local text = snapshot and shortText(snapshot) or nil
  if text ~= nil and text ~= "" then
    table.insert(children, ui.label({ text = text, color = color, maxLines = 1 }))
  end
  local container = barWidget.isVertical() and ui.column or ui.row
  barWidget.render(container({ gap = 6, align = "center" }, children))
  barWidget.setTooltip(tooltip(snapshot))
end

local function fetch()
  if busy then
    return
  end
  busy = true
  lastRun = noctalia.nowMs()
  local started = noctalia.runAsync({ command(), "--status" }, function(result)
    busy = false
    if result.timedOut then
      lastError = "timeout"
      return
    end
    if result.exitCode ~= 0 then
      lastError = result.stderr ~= "" and result.stderr or ("exit " .. tostring(result.exitCode))
      launchFailed = true
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
    launchFailed = false
    snapshot = data
    noctalia.state.set("f75_status", data)
    render()
  end, 5000)
  if not started then
    busy = false
    launchFailed = true
    render()
  end
end

noctalia.state.watch("f75_status", function(value)
  if type(value) == "table" then
    snapshot = value
    render()
  end
end)

function update()
  noctalia.setUpdateInterval(refreshSeconds() * 1000)
  -- Skip a redundant spawn if a service or the panel refreshed us very
  -- recently; the settings poll interval is the real cadence.
  if noctalia.nowMs() - lastRun > 500 then
    fetch()
  end
end

function onClick()
  if config("open_app_on_click", true) then
    noctalia.runAsync({ command() })
  else
    fetch()
  end
end

function onRightClick()
  noctalia.togglePanel("pbolkhovitin/aula-f75:details")
end

function onScroll(_axis, steps, _startsGesture)
  if steps ~= 0 then
    fetch()
  end
end

function onConfigChanged()
  fetch()
end

update()
