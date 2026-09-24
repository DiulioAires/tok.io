const SERVICE_NAME: &str = "com.diulio.aiusagewidget";

fn validate_provider(provider: &str) -> Result<(), String> {
    if ["openai", "anthropic"].contains(&provider) {
        Ok(())
    } else {
        Err("Provedor de API desconhecido.".to_string())
    }
}

fn entry(provider: &str) -> Result<keyring::Entry, String> {
    validate_provider(provider)?;
    keyring::Entry::new(SERVICE_NAME, provider)
        .map_err(|_| "O Gerenciador de Credenciais do Windows não está disponível.".to_string())
}

pub fn save(provider: &str, secret: &str) -> Result<(), String> {
    if secret.trim().is_empty() {
        return Err("A chave não pode ficar vazia.".to_string());
    }
    entry(provider)?
        .set_password(secret)
        .map_err(|_| "Não foi possível salvar a chave no Gerenciador de Credenciais.".to_string())
}

pub fn get(provider: &str) -> Result<String, String> {
    entry(provider)?
        .get_password()
        .map_err(|_| "Nenhuma chave configurada para esta fonte.".to_string())
}

pub fn configured(provider: &str) -> bool {
    get(provider).is_ok()
}

pub fn delete(provider: &str) -> Result<(), String> {
    match entry(provider)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Não foi possível apagar a chave do Gerenciador de Credenciais.".to_string()),
    }
}
