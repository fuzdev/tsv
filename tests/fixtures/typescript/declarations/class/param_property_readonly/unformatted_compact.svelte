<script lang="ts">
class A{constructor(readonly a:number){}}

class B{constructor(public readonly b:string,private readonly c=1){}}

class C{constructor(protected readonly d:string='x'){}}

// `readonly` as a plain parameter name (not a modifier — no binding follows it)
class D{constructor(readonly){}method(readonly:number){}other(readonly,x:string){}}

// `public` modifier with `readonly` as the parameter name (no binding follows)
class E{constructor(public readonly){}}
</script>
