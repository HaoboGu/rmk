import { execSync } from 'child_process'
import { readFileSync, mkdirSync, existsSync } from 'fs'

const versions = JSON.parse(readFileSync(new URL('./versions.json', import.meta.url)))
const docsRoot = 'docs'

versions.forEach((branch) => {
  const version = branch.split('/').pop()
  const targetDir = docsRoot + '/' + version

  if (!existsSync(targetDir)) {
    mkdirSync(targetDir, { recursive: true })
  }

  // Set and fetch branch
  execSync(`git remote remove rmk-origin 2>/dev/null || true`, { stdio: 'inherit', shell: 'bash' })
  execSync(`git remote add rmk-origin https://github.com/HaoboGu/rmk.git`, {
    stdio: 'inherit',
    shell: 'bash'
  })
  execSync(`git fetch rmk-origin ${branch}`, { stdio: 'inherit', shell: 'bash' })
  // Extract branch
  const command = `git archive rmk-origin/${branch} docs/main | tar -x -C ${targetDir} --strip-components=2`
  execSync(command, { stdio: 'inherit', shell: 'bash' })

  console.log(`Fetched branch ${branch} into ${targetDir}`)
})
