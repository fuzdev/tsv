<script lang="ts">
	// Comment between `function` keyword and name
	declare function/* a */f1(): void;
	export declare function/* b */f2(): void;

	// Comment before semicolon (no return type)
	declare function f3();/* c */

	// Comment before semicolon (with return type)
	declare function f4(): void/* d */;

	// Overload with keyword-name comment
	function/* e */f5(): void;
	function f5(): void {}

	// Overload with pre-semicolon comment
	function f6();/* f */
	function f6() {}

	// Multiple comments
	declare function/* g */f7();/* h */
</script>
