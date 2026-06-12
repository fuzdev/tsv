<script lang="ts">
// Line comment between name and extends
class A// comment
extends B {}

// Line comment between name and implements
class C// comment
implements D {}

// Line comment between type params and extends
class E<T>// comment
extends B {}

// Line comment between extends and implements
class F extends B// comment
implements D {}

// Block comment between name and extends
class G/* comment */extends B {}

// Block comment between type params and extends
class H<T>/* comment */extends B {}

// Multiple line comments in heritage
declare class I// 1
// 2
extends B {
// 3
}
</script>
