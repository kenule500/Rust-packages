-- Atomic quota reservation.
--
-- KEYS[1] budget key
-- KEYS[2] reservation key (tenant-scoped)
-- KEYS[3] reservation index key (id -> reservation key)
-- KEYS[4] idempotency key, or the budget key when the caller supplied none
-- KEYS[5] bucket index key (window starts)
--
-- ARGV[1]  reservation id
-- ARGV[2]  units
-- ARGV[3]  limit
-- ARGV[4]  reservation TTL in seconds
-- ARGV[5]  policy: hard_stop | overage | degrade | queue
-- ARGV[6]  unit cost in cents (overage only)
-- ARGV[7]  window end, unix milliseconds
-- ARGV[8]  window start, unix milliseconds
-- ARGV[9]  budget TTL in seconds
-- ARGV[10] caller idempotency key, or empty string
-- ARGV[11] reservation key prefix
-- ARGV[12] current time, unix milliseconds
--
-- Returns an array of strings, first element is the status:
--   OK,remaining          reserved
--   OV,cost_cents         reserved beyond the limit under the overage policy
--   DENY,remaining        hard-stop denial
--   DEGRADE / QUEUE       policy outcome, nothing held
--   ALREADY,id,units,expires_ms,policy   idempotent replay with the same units
--   CONFLICT              idempotency key reused with different units

local budget = KEYS[1]
local reservation = KEYS[2]
local reservation_index = KEYS[3]
local idem = KEYS[4]
local bucket_index = KEYS[5]

local res_id = ARGV[1]
local units = tonumber(ARGV[2])
local limit = tonumber(ARGV[3])
local ttl = tonumber(ARGV[4])
local policy = ARGV[5]
local unit_cost = tonumber(ARGV[6])
local window_end = tonumber(ARGV[7])
local window_start = ARGV[8]
local budget_ttl = tonumber(ARGV[9])
local idem_key = ARGV[10]
local res_prefix = ARGV[11]

if units <= 0 then
  return {'CONFIG', 'units must be greater than zero'}
end

if ttl < 1 then
  ttl = 1
end

-- A replay of the same idempotency key returns the original reservation, and a
-- replay with a different unit count is a typed conflict.
if idem_key ~= '' then
  local existing = redis.call('GET', idem)
  if existing then
    local rkey = redis.call('GET', 'quota:residx:' .. existing)
    if rkey then
      local stored_units = redis.call('HGET', rkey, 'units')
      if stored_units and tonumber(stored_units) == units then
        local expires = redis.call('HGET', rkey, 'expires')
        local stored_policy = redis.call('HGET', rkey, 'policy')
        local stored_cost = redis.call('HGET', rkey, 'unit_cost') or 0
        return {'ALREADY', existing, stored_units, expires, stored_policy, stored_cost}
      end
    end
    return {'CONFLICT'}
  end
end

if redis.call('EXISTS', budget) == 0 then
  redis.call('HSET', budget,
    'limit', limit,
    'used', 0,
    'held', 0,
    'overage_units', 0,
    'overage_cents', 0,
    'window_start', window_start,
    'window_end', window_end)
  redis.call('EXPIRE', budget, budget_ttl)
  redis.call('SADD', bucket_index, window_start)
  redis.call('EXPIRE', bucket_index, budget_ttl)
else
  limit = tonumber(redis.call('HGET', budget, 'limit'))
end

local used = tonumber(redis.call('HGET', budget, 'used')) or 0
local held = tonumber(redis.call('HGET', budget, 'held')) or 0
local remaining = limit - used - held
if remaining < 0 then
  remaining = 0
end

local overage_cost = 0
local overage = false

if units > remaining then
  if policy == 'overage' then
    overage = true
    overage_cost = units * unit_cost
  elseif policy == 'degrade' then
    return {'DEGRADE'}
  elseif policy == 'queue' then
    return {'QUEUE'}
  else
    return {'DENY', tostring(remaining)}
  end
end

redis.call('HINCRBY', budget, 'held', units)
local now_ms = tonumber(ARGV[12])
redis.call('HSET', reservation,
  'units', units,
  'policy', policy,
  'unit_cost', unit_cost,
  'expires', now_ms + ttl * 1000,
  'budget', budget,
  'overage_cents', overage_cost,
  'settled', 0)
redis.call('EXPIRE', reservation, ttl)
redis.call('SET', reservation_index, reservation, 'EX', ttl)

if idem_key ~= '' then
  redis.call('SET', idem, res_id, 'EX', ttl)
end

if overage then
  return {'OV', tostring(overage_cost)}
end
return {'OK', tostring(remaining - units)}
