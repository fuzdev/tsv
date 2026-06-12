<script lang="ts">
// leading block comment after comma
import {a,/* c */b} from './a';

// trailing block comment before comma
import {c/* c */,d} from './b';

// both trailing and leading around comma
import {e/* c1 */,/* c2 */f} from './c';

// multiple specifiers with comments
import {g,/* c1 */h,/* c2 */i} from './d';

// export: leading block comment after comma
export {a,/* c */b};

// export: trailing block comment before comma
export {c/* c */,d};

// export re-export with comment
export {e,/* c */f} from './e';
</script>
