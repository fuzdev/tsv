<script lang="ts">
// 100 chars at 1-tab: entire statement stays inline
const a = cccccc.ddddddddddddddddd.eeeeeeeeeeeeeee.filter((x) => x.s === 'a' || x.s === 'bbbbbb');

// 101 chars at 1-tab: filter args expand
const b = cccccc.ddddddddddddddddd.eeeeeeeeeeeeeee.filter((x) => x.s === 'a' || x.s === 'bbbbbbb');

// 100 chars at 5-tab opening line: filter args still expanded
function fn(){if(true){if(true){if(true){const eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee = cccccc.ddddddddddddddddd.eeeeeeeeeeeeeee.filter((x) => x.s === 'a' || x.s === 'b');}}}}

// 101 chars at 5-tab opening line: assignment breaks
function gn(){if(true){if(true){if(true){const eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee = cccccc.ddddddddddddddddd.eeeeeeeeeeeeeee.filter((x) => x.s === 'a' || x.s === 'b');}}}}
</script>
