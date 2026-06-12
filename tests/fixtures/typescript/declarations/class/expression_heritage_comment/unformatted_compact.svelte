<script lang="ts">
// Line comment between name and extends
let a = class A// comment
extends B {};

// Block comment between name and extends
let b = class C/* comment */extends B {};

// Block comment between superclass and body
let c = class D extends B/* comment */{};

// Block comment between superclass and body, anonymous
let d = class extends B/* comment */{};
</script>
