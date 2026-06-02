<script lang="ts">
// Variable annotation: comment on own line before second member
let v:A&
// comment before B
B;

// Variable annotation: comment trailing the &
let w:A&// trailing
B;

// Class property: comment between members
class C{p:A&
// comment before B
B;}

// Function parameter: comment between members
function fn(x:A&
// comment before B
B){}
</script>
