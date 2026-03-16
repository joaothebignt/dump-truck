<script setup>

import { computed } from "vue"

const props = defineProps({
  tables: Array,
  filter: String
})

const filteredTables = computed(() => {

  if (!props.tables)
    return []

  return props.tables.filter(t =>
    t.name.includes(props.filter)
  )

})

function formatSize(bytes){

 if(bytes < 1024)
  return bytes+" B"

 if(bytes < 1024*1024)
  return (bytes/1024).toFixed(1)+" KB"

 return (bytes/1024/1024).toFixed(2)+" MB"

}

</script>

<template>

<table>

<tr
v-for="t in filteredTables"
:key="t.name"
>

<td>{{ t.name }}</td>

<td>{{ formatSize(t.size) }}</td>

<td>
<input type="checkbox" v-model="t.selected">
</td>

</tr>

</table>

</template>