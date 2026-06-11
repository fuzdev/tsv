<script lang="ts">
// Basic optional properties
class A{a?:string;b?:number;}

// Readonly optional
class B{readonly a?:string;}

// Static optional
class C{static a?:string;}

// Private optional
class D{#a?:number;}

// Access modifiers with optional
class E{public a?:string;private b?:number;protected c?:boolean;}

// Abstract optional
abstract class F{abstract a?:string;}

// Generic class with optional
class G<T>{a?:T;}

// Definite assignment assertion (!)
class H{a!:string;}
// Without type annotation (implicit any)
class I{a?;b!;}
</script>
