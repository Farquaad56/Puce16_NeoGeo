-- Script MAME (0.287) : enregistre des captures d'écran PNG à des numéros de trame précis.
-- Paramètres via variables d'environnement (MAME ne passe pas d'arguments aux scripts) :
--   NG_FRAMES = "120,600,1500"   (numéros de trame, 1 = première trame émulée)
--   NG_PREFIX = "f"              (préfixe des fichiers ; sortie : <snapshot_directory>/<prefix><N>.png)
local frames = {}
local last = 0
for n in string.gmatch(os.getenv("NG_FRAMES") or "120", "%d+") do
  local v = tonumber(n); frames[v] = true; if v > last then last = v end
end
local prefix = os.getenv("NG_PREFIX") or "f"
local count = 0
local screen = nil
for _, s in pairs(manager.machine.screens) do screen = s; break end
emu.add_machine_frame_notifier(function()
  count = count + 1
  if frames[count] then
    screen:snapshot(string.format("%s%06d.png", prefix, count))
  end
  if count >= last then manager.machine:exit() end
end)
