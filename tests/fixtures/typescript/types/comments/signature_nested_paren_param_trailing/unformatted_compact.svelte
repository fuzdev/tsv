<script lang="ts">
	// type literal method signature
	type T = {m(cb:(x:number)=>void/* c */):void};

	// interface method signature
	interface I1 {m(cb:(x:number)=>void/* c */):void;}

	// interface call signature
	interface I2 {(cb:(x:number)=>void/* c */):void;}

	// interface construct signature
	interface I3 {new (cb:(x:number)=>void/* c */):void;}

	// declare function
	declare function fn(cb:(x:number)=>void/* c */):void;
</script>
