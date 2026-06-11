<script lang="ts">
	// `this` parameter in a function type
	type T1=(this:A)=>void;

	// `this` followed by regular parameters
	type T2=(this:A,b:B)=>void;

	// `this` followed by a rest parameter
	type T3=(this:A,...b:B[])=>void;

	// function type with `this` param as an object-type property value
	type T4={
		f:(this:A)=>void;
	};
</script>
