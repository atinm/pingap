import { LoadingPage } from "@/components/loading";
import { useI18n } from "@/i18n";
import useConfigState, { Certificate } from "@/states/config";
import React from "react";
import { ExForm, ExFormItem } from "@/components/ex-form";
import { z } from "zod";
import {
  ExFormItemCategory,
  newStringOptions,
  newBooleanOptions,
} from "@/constants";
import { useSearchParams } from "react-router-dom";
import { useEffect } from "react";
import { formatLabel } from "@/helpers/util";
import { useShallow } from "zustand/react/shallow";

function getCertificateConfig(
  name: string,
  certificates?: Record<string, Certificate>,
) {
  if (!certificates) {
    return {} as Certificate;
  }
  return (certificates[name] || {}) as Certificate;
}

export default function Certificates() {
  const certificateI18n = useI18n("certificate");
  const [searchParams, setSearchParams] = useSearchParams();

  const [config, initialized, update, remove] = useConfigState(
    useShallow((state) => [
      state.data,
      state.initialized,
      state.update,
      state.remove,
    ]),
  );
  const newCertificate = "*";
  const certificates = Object.keys(config.certificates || {});
  certificates.sort();
  certificates.unshift(newCertificate);
  const [currentCertificate, setCurrentCertificate] = React.useState(
    searchParams.get("name") || newCertificate,
  );
  useEffect(() => {
    setCurrentCertificate(searchParams.get("name") || newCertificate);
  }, [searchParams]);
  if (!initialized) {
    return <LoadingPage />;
  }

  const handleSelectCertificate = (name: string) => {
    setCurrentCertificate(name);
    if (name === newCertificate) {
      searchParams.delete("name");
    } else {
      searchParams.set("name", name);
    }
    setSearchParams(searchParams);
  };

  const certificateConfig = getCertificateConfig(
    currentCertificate,
    config.certificates,
  );
  const countLines = (value: string) => {
    const count = value.split("\n").length;
    return Math.min(Math.max(3, count), 8);
  };

  const items: ExFormItem[] = [
    {
      name: "tls_cert",
      label: certificateI18n("tlsCert"),
      placeholder: certificateI18n("tlsCertPlaceholder"),
      defaultValue: certificateConfig.tls_cert,
      span: 6,
      category: ExFormItemCategory.TEXTAREA,
      rows: countLines(certificateConfig.tls_cert || ""),
    },
    {
      name: "tls_key",
      label: certificateI18n("tlsKey"),
      placeholder: certificateI18n("tlsKeyPlaceholder"),
      defaultValue: certificateConfig.tls_key,
      span: 6,
      category: ExFormItemCategory.TEXTAREA,
      rows: countLines(certificateConfig.tls_key || ""),
    },
    {
      name: "tls_chain",
      label: certificateI18n("tlsChain"),
      placeholder: certificateI18n("tlsChainPlaceholder"),
      defaultValue: certificateConfig.tls_chain,
      span: 6,
      category: ExFormItemCategory.TEXTAREA,
    },
    {
      name: "domains",
      label: certificateI18n("domains"),
      placeholder: certificateI18n("domainsPlaceholder"),
      defaultValue: certificateConfig.domains,
      span: 6,
      category: ExFormItemCategory.TEXT,
    },
    {
      name: "acme",
      label: certificateI18n("acme"),
      placeholder: "",
      defaultValue: certificateConfig.acme,
      span: 3,
      category: ExFormItemCategory.RADIOS,
      options: newStringOptions(["lets_encrypt"], true, true),
    },
    {
      name: "is_default",
      label: certificateI18n("isDefault"),
      placeholder: "",
      defaultValue: certificateConfig.is_default,
      span: 3,
      category: ExFormItemCategory.RADIOS,
      options: newBooleanOptions(),
    },
    {
      name: "is_root",
      label: certificateI18n("isRoot"),
      placeholder: "",
      defaultValue: certificateConfig.is_root,
      span: 3,
      category: ExFormItemCategory.RADIOS,
      options: newBooleanOptions(),
    },
  ];

  let defaultShow = 2;
  if (currentCertificate === newCertificate) {
    defaultShow++;
    items.unshift({
      name: "name",
      label: certificateI18n("name"),
      placeholder: certificateI18n("namePlaceholder"),
      defaultValue: "",
      span: 6,
      category: ExFormItemCategory.TEXT,
    });
  }
  const schema = z.object({});
  const onRemove = async () => {
    return remove("certificate", currentCertificate).then(() => {
      handleSelectCertificate(newCertificate);
    });
  };

  return (
    <div className="grow lg:border-l overflow-auto p-4">
      <h2 className="h-8 mb-1">
        <span className="border-b-2 border-solid py-1 border-[rgb(var(--foreground-rgb))]">
          {formatLabel(currentCertificate)}
        </span>
      </h2>
      <ExForm
        category="certificate"
        key={currentCertificate}
        items={items}
        schema={schema}
        defaultShow={defaultShow}
        onRemove={currentCertificate === newCertificate ? undefined : onRemove}
        onSave={async (value) => {
          let name = currentCertificate;
          if (name === newCertificate) {
            name = value["name"] as string;
          }
          await update("certificate", name, value);
          handleSelectCertificate(name);
        }}
      />
    </div>
  );
}
