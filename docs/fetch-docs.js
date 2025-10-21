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

  // Fetch origin branch first
  execSync(`git fetch origin ${branch}`, { stdio: 'inherit', shell: 'bash' })
  // Extract branch
  const command = `git archive origin/${branch} docs/main | tar -x -C ${targetDir} --strip-components=2`
  execSync(command, { stdio: 'inherit', shell: 'bash' })

  console.log(`Fetched branch ${branch} into ${targetDir}`)
})
