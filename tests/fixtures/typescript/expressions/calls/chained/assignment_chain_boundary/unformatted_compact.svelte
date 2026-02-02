<script lang="ts">
// Assignment + chain boundary: when `const xxx = aaa.bbb.ccc.filter(` hits exactly 100/101 chars
// At 4 tabs (8 visual), we test where the break point changes
// TODO: match prettier's break-at-assignment behavior for chains

function outer(){if(true){if(true){if(true){
// 100 chars total: fits on one line, only args break internally
const aaaa_bbbbbbbbbbbbbbbbbbbbbbbbbbbb=cccccc.ddddddddddddddddd.eeeeeeeeeeeeeee.filter((x)=>x.s==='a');
// 101 chars total: we expand call args, prettier breaks after =
const aaaa_bbbbbbbbbbbbbbbbbbbbbbbbbbbbb=cccccc.ddddddddddddddddd.eeeeeeeeeeeeeee.filter((x)=>x.s==='a');
}}}}
</script>
