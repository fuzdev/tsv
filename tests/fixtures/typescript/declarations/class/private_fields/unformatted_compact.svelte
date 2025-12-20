<script lang="ts">
// Private fields and methods
class A{#a=0;#b(){}}

// Private field access
class B{#a=1;get(){return this.#a;}}

// Optional chaining with private fields
class C{#a=0;access(x?:C){return x?.#a;}}

// Static private fields
class D{static #a=0;static fn(){D.#a++;}}

// Typed private fields
class E{#a:string;#b:number=0;}

// Private getters/setters
class F{#a=0;get #b(){return this.#a;}set #c(v:number){this.#a=v;}}

// Private brand check (in operator)
class G{#a;static check(x:object){return #a in x;}}

// Static private methods
class H{static #a(){return 1;}static b(){return H.#a();}}

// Async/generator private methods
class I{async #a(){}*#b(){}async *#c(){}}

// Private fields in expressions
class J{#a=1;#b=2;fn(){return this.#a+this.#b;}}
</script>
