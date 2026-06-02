<script lang="ts">
// Single type-arg superclass — declaration and expression render identically
class A extends Base<T>{}
const B=class extends Base<T>{};

// Object-literal type arg hugs the angle bracket — both forms
class C extends Base<{a:string;b:number}>{}
const D=class extends Base<{a:string;b:number}>{};

// Multiple type args — both forms
class E extends Base<T,U>{}
const F=class extends Base<T,U>{};
</script>
