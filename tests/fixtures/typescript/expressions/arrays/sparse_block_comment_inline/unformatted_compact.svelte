<script>
// Block comment between elision commas — single line
const a=[,,,,/* c */];

// Two block comments separated by elision commas
const b=[
,/* c1 */
/* c2 */
];

// Block comment leading on a real element after holes
const c=[,,/* c */1];

// Block comment trailing on a real element before holes
const d=[1/* c */,,,];
</script>
