<script lang="ts">
	// type parameter constraint
	type A<T extends(// leading
	a)>=T;

	// tuple element
	type B=[(// leading
	a),b];

	// generic type argument
	type C=Array<(// leading
	a)>;

	// function type return
	type D=()=>(// leading
	a);
</script>
