<script lang="ts">
class A {
// Comment between name and !: (comment before modifier)
a/* c1 */!:number;

// Comment between name and ?: (comment before modifier)
b/* c2 */?:number;

// Comment between ! and : (comment after modifier)
c!/* c3 */:number;

// Comment between ? and : (comment after modifier)
d?/* c4 */:number;
}
</script>
