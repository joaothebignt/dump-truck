<script setup>

import { onMounted, reactive, ref } from "vue"
import { useRoute, useRouter } from "vue-router"
import { invoke } from "@tauri-apps/api/core"

import { useConnectionsStore } from "../stores/connections"

const router = useRouter()
const route = useRoute()
const store = useConnectionsStore()

const editingIndex = ref(null)
const testing = ref(false)
const testMessage = ref(null)

const form = reactive({

  name: "",
  host: "127.0.0.1", // pode ser IP ou hostname na rede local
  port: 3306,
  username: "",
  password: "",
  color: "#3a7afe",
  saveFavorite: false

})

onMounted(() => {

  if (route.params.index !== undefined) {

    const idx = Number(route.params.index)
    const existing = store.favorites[idx]

    if (existing) {

      editingIndex.value = idx

      form.name = existing.name
      form.host = existing.host
      form.port = existing.port
      form.username = existing.username
      form.password = existing.password
      form.color = existing.color || "#3a7afe"
      form.saveFavorite = true

    }
  }

})

async function testConnection() {
  if (!form.host?.trim()) {
    testMessage.value = { ok: false, text: "Informe o host." }
    return
  }
  testing.value = true
  testMessage.value = null
  try {
    await invoke("connect_db", {
      host: form.host.trim(),
      port: Number(form.port) || 3306,
      user: form.username || "",
      pass: form.password || ""
    })
    testMessage.value = { ok: true, text: "Conexão OK." }
  } catch (e) {
    testMessage.value = { ok: false, text: String(e) }
  } finally {
    testing.value = false
  }
}

function save() {

  if (editingIndex.value !== null) {

    store.update(editingIndex.value, { ...form })

  } else if (form.saveFavorite) {

    store.add({ ...form })

  }

  router.push("/")

}

</script>

<template>

<h2>{{ editingIndex !== null ? "Editar conexão" : "Nova conexão" }}</h2>

<input v-model="form.name" placeholder="Nome da conexão" />
<input v-model="form.host" placeholder="Host (ex: 192.168.0.10)" />
<input v-model="form.port" placeholder="Port" />

<input v-model="form.username" placeholder="Username" />

<input
  type="password"
  v-model="form.password"
  placeholder="Password"
/>

<div class="row">
  <label>
    Cor:
    <input
      v-model="form.color"
      type="color"
    >
  </label>
</div>

<label class="row">
  <input
    v-model="form.saveFavorite"
    type="checkbox"
  >
  Salvar como favorita
</label>

<div v-if="testMessage" :class="['test-message', testMessage.ok ? 'success' : 'error']">
  {{ testMessage.text }}
</div>

<button @click="testConnection" :disabled="testing">
  {{ testing ? "Testando…" : "Testar conexão" }}
</button>

<button @click="save">
  Salvar
</button>

<button @click="router.push('/')">
  Cancelar
</button>

</template>

<style scoped>
.test-message {
  padding: 8px 10px;
  border-radius: 4px;
  font-size: 14px;
  margin-bottom: 8px;
}
.test-message.success {
  background: rgba(56, 189, 124, 0.2);
  color: #38bd7c;
}
.test-message.error {
  background: rgba(248, 113, 113, 0.2);
  color: #f87171;
}
</style>