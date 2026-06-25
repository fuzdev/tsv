<script lang="ts">
// Named class expression with implements (no extends)
const A=class Named implements Contract{};
const B=class Named implements Contract{prop:string;};

// Generic class expression with implements (no extends)
const C=class<T> implements Contract<T>{};

// Named generic with multiple interfaces
const D=class Named<T> implements Contract1<T>,Contract2{};
</script>
