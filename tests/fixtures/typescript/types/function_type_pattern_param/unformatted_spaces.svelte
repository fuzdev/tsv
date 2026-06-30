<script lang="ts">
	// Destructuring-pattern parameters in function and constructor types,
	// both required and optional.
	type F1 = ( [] ) => void;
	type F2 = ( [] ? ) => void;
	type F3 = ( [ a ] : number[] ) => void;
	type F4 = ( {} ? ) => void;
	type C1 = new ( [] ? ) => void;
	type C2 = new ( { a } : T ) => void;
</script>
