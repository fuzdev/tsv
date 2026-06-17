<script lang="ts">
// Union member object with interior leading comment: hugs inline, keeps the comment
type T={/* c */ a:1}|void;

// Multiple properties: still hugs
type M={/* c */ a:1;b:2}|null;

// Three union members: hugs, non-object members appended
type N={/* c */ a:1}|null|void;

// Intersection sibling already hugs (consistency target)
type U={/* c */ a:1}&B;
</script>
