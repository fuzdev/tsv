<script lang="ts">
// Variable declarations with definite assignment
let a!:string;
var b!:number;

// Multiple declarations
let c!:A,d!:B;

// Class property with definite assignment
class Single{prop!:string;}

// Class with multiple definite assignment properties
class Multiple{a!:A;b!:B;}

// Static class property
class WithStatic{static prop!:string;}

// Access modifiers
class WithAccess{private a!:string;protected b!:number;public c!:boolean;}

// Readonly modifier
class WithReadonly{readonly prop!:string;}

// Combined modifiers
class WithCombined{private readonly a!:string;static readonly b!:number;}

// Private field (ES2022)
class WithPrivateField{#prop!:string;}
</script>
