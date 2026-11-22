-- Atomic settlement of one reservation.
--
-- KEYS[1] reservation index key (reservation id -> tenant-scoped reservation key)
-- ARGV[1] mode: commit | release
--
-- Returns an array of strings, first element is the status:
--   OK       settled by this call
--   ALREADY  already settled; the counters are not touched again
--   MISSING  no reservation for this id (expired or never existed)

local idx = KEYS[1]
local mode = ARGV[1]

local rkey = redis.call('GET', idx)
if not rkey then
  return {'MISSING'}
end
if redis.call('EXISTS', rkey) == 0 then
  return {'MISSING'}
end
if redis.call('HGET', rkey, 'settled') == '1' then
  return {'ALREADY'}
end

local units = tonumber(redis.call('HGET', rkey, 'units')) or 0
local policy = redis.call('HGET', rkey, 'policy')
local budget = redis.call('HGET', rkey, 'budget')
local cost = tonumber(redis.call('HGET', rkey, 'overage_cents')) or 0

redis.call('HSET', rkey, 'settled', 1)

if budget then
  if mode == 'commit' then
    redis.call('HINCRBY', budget, 'used', units)
    redis.call('HINCRBY', budget, 'held', -units)
    if policy == 'overage' and cost > 0 then
      redis.call('HINCRBY', budget, 'overage_units', units)
      redis.call('HINCRBY', budget, 'overage_cents', cost)
    end
  else
    redis.call('HINCRBY', budget, 'held', -units)
  end
end

return {'OK'}
