<script lang="ts">
	// A parameter list opens a fresh frame: an arrow in a consequent arrow's parameter
	// default keeps its return type though no second `:` follows it
	const s1=a?(x=(y):T=>z)=>w:v;

	// Beside another parameter, and in an async arrow's list
	const s2=a?(x=(y):T=>z,u)=>w:v;
	const s3=a?async(x=(y):T=>z)=>w:v;

	// The consequent arrow keeping a return type of its own
	const s4=a?(x=(y):T=>z):R=>w:v;

	// A default after a function-typed annotation
	const s5=a?(x:(y:T)=>U=(y):T=>z)=>w:v;

	// As a call argument
	fn(a?(x=(y):T=>z)=>w:v);
</script>
