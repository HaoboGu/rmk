import { execSync } from 'child_process'
import { readFileSync, mkdirSync, existsSync } from 'fs'
import { join } from 'path'

const versions = JSON.parse(readFileSync(new URL('./versions.json', import.meta.url)))

const docsRoot = 'docs/docs'

versions.forEach((branch) => {
  const version = branch.split('/').pop()
  
  const targetDir = join(docsRoot, version)

  if (!existsSync(targetDir)) {
    mkdirSync(targetDir, { recursive: true }) 
  }

  // Fetch origin branch first
  execSync(`git fetch origin ${branch}`, { stdio: 'inherit', shell: 'bash' });
  const command = `git archive origin/${branch} docs/main | tar -x -C ${targetDir} --strip-components=3`
  
  console.log(`Fetched branch ${branch} into ${targetDir}`)
  execSync(command, { stdio: 'inherit', shell: 'bash' })
})
