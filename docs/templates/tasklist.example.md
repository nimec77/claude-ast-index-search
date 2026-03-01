# T-104: GET /invoices/{id}

Status: TASKLIST_READY

Context: PRD §3.2; plan docs/plan/T-104.md

- [ ] OpenAPI: docs/api/T-104.yaml
- Endpoint description, error codes, examples.
- [ ] Controller: InvoiceController.getById
- Error handling, DTO mapping.
- [ ] Service: InvoiceService.findByIdWithPermissions
- Permission check + repository access.
- [ ] IT tests
- Positive and negative scenarios.
