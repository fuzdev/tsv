<script lang="ts">
	// Rest element as an array / object binding pattern
	function f(...[a,b]){}
	function g(...{a}){}

	// Empty patterns
	function h(...[]){}
	function i(...{}){}

	// Nested patterns
	function j(...[a,[b],{c}]){}

	// Typed: the annotation binds the rest element, not the inner pattern
	function k(...[a,b]:number[]){}
	function l(...{a}:{a:number}){}
</script>
