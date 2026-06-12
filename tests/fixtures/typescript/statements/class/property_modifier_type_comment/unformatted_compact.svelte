<script lang="ts">
class A {
// Comment between name and !: (comment before modifier)
a/* c1 */!:number=1;

// Comment between name and ?: (comment before modifier)
b/* c2 */?:number=1;

// Comment between ! and : (comment after modifier)
c!/* c3 */:number=1;

// Comment between ? and : (comment after modifier)
d?/* c4 */:number=1;
}
</script>
