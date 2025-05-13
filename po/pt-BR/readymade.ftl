prev = Anterior
next = Próximo

unknown-os = S.O. desconhecido

parttype-root = Sistema de arquivos raiz ({$path})
parttype-extendedboot = Partição extendida do carregador de inicialização ({$path})
parttype-esp = Partição do Sistema EFI ({$path})
parttype-home = Arquivos do Usuário ({$path})
parttype-var = Arquivos variáveis ({$path})
parttype-other = Ponto de montagem de particionamento customizado


page-welcome = Bem-Vindo a(o) {$distro}
page-welcome-desc = Você pode tentar {$distro} ou iniciar a instalação agora.
page-welcome-try = Tentar
page-welcome-install = Instalar

page-failure = Falha na instalação
page-failure-close = Fechar
page-failure-bug = Reporte uma falha

page-language = Linguagem
page-language-search-lang = Pesquisar idioma/localidade
page-language-next = Próximo

page-completed = Completo
page-completed-desc = Instalação completa. Você pode reiniciar agora e aproveitar seu novo sistema.
page-completed-close = Fechar
page-completed-reboot = Reiniciar

page-destination = Destino
page-destination-scanning = Procurando Discos
page-destination-wait = Esperando o os-prober…
page-destination-no-disk = Nenhum disco encontrado
page-destination-no-disk-desc = Não foram encontrados discos adequados para instalação.

page-installdual = Dual Boot
page-installdual-otheros = Outro S.O.

page-confirmation = Confirmar
page-confirmation-problem-device-mounted = {$dev} está montado em {$mountpoint}. Desmonte para prosseguir.
page-confirmation-problem-devblkopen = O dispositivo <tt>{$dev}</tt> está em uso pelos seguintes processos:
    <tt>{$pids}</tt>
    Esses processos devem ser fechados antes que o instalador prossiga. 

page-installation = Instalação
page-installation-welcome-desc = Conheça seu novo Sistema Operacional.
page-installation-help = Precisa de ajuda?
page-installation-help-desc = Pergunte em um de nossos chats!
page-installation-contrib = Contribua para {$distro}
page-installation-contrib-desc = Aprenda como contribuir com o seu tempo, dinheiro, ou hardware.
page-installation-progress = Instalando o sistema...

page-installcustom = Instalação customizada
page-installcustom-title = Partições e Pontos de Montagem
page-installcustom-desc = {$num} definição(ões)
page-installcustom-tool = Abrir a ferramenta de particionamento
page-installcustom-add = Adicione uma nova definição/linha

page-installationtype = Tipo de instalação
page-installationtype-entire = Disco Inteiro
page-installationtype-tpm = Habilitar TPM
page-installationtype-encrypt = Habilitar criptografia do disco
page-installationtype-chromebook = Chromebook
page-installationtype-dual = Dual Boot
page-installationtype-custom = Customizado

dialog-installtype-encrypt = Criptografia de disco
dialog-installtype-encrypt-desc = Por favor, defina a senha de criptografia do disco.
    Se você esquecer a senha, seus dados não serão recuperáveis.
dialog-installtype-password = Senha
dialog-installtype-repeat = Repita a senha
dialog-installtype-cancel = Cancelar
dialog-installtype-confirm = Confirmar

installtype-edit-mp = Mudar o ponto de montagem
installtype-rm-mp = Remover o ponto de montagem

dialog-mp-part = Partição
dialog-mp-at = Montar em
dialog-mp-opts = Opções de montagem

installtype-parttool = Selecione sua ferramenta de particionamento

stage-extracting = Extraindo arquivos
stage-copying = Copiando arquivos
stage-mkpart = Criando partições e copiando arquivos
stage-initramfs = Regenerando a initramfs
stage-grub = Gerando padrões do grub do sistema
stage-grub1 = Gerando o grub.cfg de estágio 1 no ESP...
stage-grub2 = Gerando o grub.cfg de estágio 2 no /boot/grub2/grub.cfg...
stage-biosgrub = Instalando Grub2 de BIOS 
stage-kernel = Reinstalando kernels
stage-selinux = Configurando rótulos SELinux


