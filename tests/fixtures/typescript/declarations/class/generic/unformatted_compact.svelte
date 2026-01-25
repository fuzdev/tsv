<script lang="ts">
//
// Generic Classes
//

// Single type parameter
class Single<T>{value:T;constructor(value:T){this.value=value;}}

// Multiple type parameters
class Multiple<T,U>{a:T;b:U;constructor(a:T,b:U){this.a=a;this.b=b;}}

// With constraint
class WithConstraint<T extends object>{items:T[]=[];add(item:T):void{this.items.push(item);}}

// With default
class WithDefault<T=number>{items:T[]=[];push(item:T):void{this.items.push(item);}}

// Extends generic base
class Extends<T> extends Single<T>{get():T|undefined{return this.value;}}

// Complex: constraint referencing other param
class Complex<T,U extends T> extends Single<U>{base:T|undefined;}

// Implements generic interface
class Implements<T> implements Contract<T>{value:T;constructor(value:T){this.value=value;}}

// Class expression (anonymous)
const Expr=class<T>{value:T;constructor(value:T){this.value=value;}};

// Class expression (named)
const Named=class Inner<T>{value:T;};
</script>
