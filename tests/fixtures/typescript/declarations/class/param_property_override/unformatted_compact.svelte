<script lang="ts">
	class A extends Base {
		constructor( override   a:number ){ super(); }
	}

	class B extends Base {
		constructor( public override b:string, protected override readonly c:boolean ){ super(); }
	}

	// override before a default value: the parameter node's span starts at the
	// first modifier (acorn quirk) when there is no type annotation
	class C extends Base {
		constructor( private override d=1, public override readonly e:string='x' ){ super(); }
	}

	// `override` as a plain parameter name (not a modifier — no binding follows it)
	class D {
		constructor( override ){}
		method( override:number ){}
		other( override, x:string ){}
	}
</script>
