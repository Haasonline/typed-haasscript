-- SYNTAX TEST "source.haasscript" "a plain HaasScript file"
local prices = ClosePrices()
-- <----- storage.modifier.local.haasscript
--             ^^^^^^^^^^^ entity.name.function.call.haasscript
local average = SMA(prices, 10)
--                          ^^ constant.numeric.decimal.haasscript
--[[ a block
--   ^^^^^^^ comment.block.haasscript
     comment ]]
--   ^^^^^^^ comment.block.haasscript
local text = [==[a long
--           ^^^^^^^^^^ string.quoted.other.multiline.haasscript
string]==]
-- <---------- string.quoted.other.multiline.haasscript
if average[1] ~= nil and not done then
-- <-- keyword.control.haasscript
--            ^^ keyword.operator.comparison.haasscript
--               ^^^ constant.language.haasscript
--                       ^^^ keyword.operator.logical.haasscript
    Log("value: " .. average[1])
--      ^^^^^^^^^ string.quoted.double.haasscript
--       ^^^^^ - entity.name.type.typed-haasscript
end
