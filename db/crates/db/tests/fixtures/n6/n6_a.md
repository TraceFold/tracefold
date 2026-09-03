# N6 contention source A

## N6-A concurrent push source A [MEASURED]
This file exists so that control N6 can start two `db push` processes that each admit exactly one
new atom and therefore contend for the same next sequence number. It is data, not engine code.
