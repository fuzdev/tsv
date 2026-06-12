<script lang="ts">
// Block comment inside the superclass call expression — preserved
class A extends mixin(/* c */Base){}

// Same superclass expression in a class expression — preserved identically
const B=class extends mixin(/* c */Base){};

// Comment inside a member-chain superclass — preserved
class C extends getMixin(Base/* c */).Inner{}
</script>
