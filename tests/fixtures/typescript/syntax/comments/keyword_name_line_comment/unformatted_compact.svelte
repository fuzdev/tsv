<script lang="ts">
// class keyword to name — block comment stays inline (both formatters)
class/* c2 */B {}

// function expression keyword to name — an expression (not a declaration
// header), so a line comment stays flat in both formatters
const a = function// c3
f1() {};

const b = function/* c4 */f2() {};

// declare function keyword to name — block comment stays inline (both formatters)
declare function/* c6 */f4(): void;
</script>
