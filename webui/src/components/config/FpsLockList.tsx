"use client";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { FpsLockItem } from "./FpsLockItem";
import type { FpsLockList as FpsLockListType } from "@/types/config";
import { Plus, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useRef, useState, useEffect } from "react";
import { Combobox } from "@/components/ui/combobox";
import { useQuery } from "@tanstack/react-query";
import { fetchApps } from "@/lib/api";

interface FpsLockListProps {
  fpsLock: FpsLockListType;
  newFpsLockPackage: string;
  setNewFpsLockPackage: (value: string) => void;
  newFpsLockMin: string;
  setNewFpsLockMin: (value: string) => void;
  newFpsLockMax: string;
  setNewFpsLockMax: (value: string) => void;
  isAddingFpsLock: boolean;
  setIsAddingFpsLock: (value: boolean) => void;
  editingFpsLock: string | null;
  editingFpsLockMin: string;
  editingFpsLockMax: string;
  setEditingFpsLockMin: (value: string) => void;
  setEditingFpsLockMax: (value: string) => void;
  addNewFpsLock: () => void;
  removeFpsLock: (game: string) => void;
  startEditFpsLock: (game: string, fpsLock: [number, number]) => void;
  saveEditedFpsLock: () => void;
}

export function FpsLockList({
  fpsLock,
  newFpsLockPackage,
  setNewFpsLockPackage,
  newFpsLockMin,
  setNewFpsLockMin,
  newFpsLockMax,
  setNewFpsLockMax,
  isAddingFpsLock,
  setIsAddingFpsLock,
  editingFpsLock,
  editingFpsLockMin,
  editingFpsLockMax,
  setEditingFpsLockMin,
  setEditingFpsLockMax,
  addNewFpsLock,
  removeFpsLock,
  startEditFpsLock,
  saveEditedFpsLock,
}: FpsLockListProps) {
  const { t } = useTranslation();
  const [isPopupVisible, setIsPopupVisible] = useState(false);
  const minInputRef = useRef<HTMLInputElement>(null);
  const { data: apps = [], isLoading } = useQuery({
    queryKey: ["apps"],
    queryFn: fetchApps,
  });

  useEffect(() => {
    if (isAddingFpsLock) {
      setTimeout(() => {
        setIsPopupVisible(true);
      }, 50);
    } else {
      setIsPopupVisible(false);
    }
  }, [isAddingFpsLock]);

  return (
    <div className="relative">
      <Card className="shadow-sm border border-border/40">
        <CardHeader className="pb-2 border-b border-border/20 flex flex-row items-center justify-between">
          <div>
            <CardTitle className="text-lg font-bold">
              {t("common:fps_lock")}
            </CardTitle>
            <CardDescription>{t("common:fps_lock_desc")}</CardDescription>
          </div>
          {!isAddingFpsLock && (
            <Button
              onClick={() => setIsAddingFpsLock(true)}
              size="icon"
              className="h-8 w-8 rounded-full bg-green-500 hover:bg-green-600 text-white"
            >
              <Plus className="h-4 w-4" strokeWidth={5} />
            </Button>
          )}
        </CardHeader>

        <CardContent className="p-4 space-y-4">
          <div className="space-y-4 overflow-x-auto">
            <div className="grid grid-cols-1 gap-4">
              {Object.entries(fpsLock).length === 0 ? (
                <div className="text-center py-8 text-muted-foreground">
                  {t("common:no_fps_lock")}
                </div>
              ) : (
                Object.entries(fpsLock).map(([game, lock]) => (
                  <FpsLockItem
                    key={game}
                    game={game}
                    fpsLock={lock}
                    editingFpsLock={editingFpsLock}
                    editingFpsLockMin={editingFpsLockMin}
                    editingFpsLockMax={editingFpsLockMax}
                    setEditingFpsLockMin={setEditingFpsLockMin}
                    setEditingFpsLockMax={setEditingFpsLockMax}
                    startEditFpsLock={startEditFpsLock}
                    saveEditedFpsLock={saveEditedFpsLock}
                    removeFpsLock={removeFpsLock}
                  />
                ))
              )}
            </div>
          </div>
        </CardContent>
      </Card>

      {isAddingFpsLock && (
        <div
          className={`fixed top-0 left-0 right-0 z-50 mx-auto max-w-md p-4 transition-opacity duration-300 ${
            isPopupVisible ? "opacity-100" : "opacity-0"
          }`}
        >
          <div
            className="fixed inset-0 bg-black/80 backdrop-blur-sm z-40 transition-opacity duration-300"
            style={{ opacity: isPopupVisible ? 1 : 0 }}
            onClick={() => setIsAddingFpsLock(false)}
          />

          <Card className="bg-card border-border shadow-lg mb-4 relative z-50 rounded-xl">
            <Button
              onClick={() => setIsAddingFpsLock(false)}
              variant="ghost"
              size="icon"
              className="absolute right-2 top-2 h-8 w-8 rounded-full bg-muted/50 hover:bg-muted text-foreground border-none"
            >
              <X className="h-4 w-4" />
            </Button>

            <CardHeader className="pb-2 pt-4">
              <CardTitle className="text-xl text-foreground">
                {t("common:add_fps_lock")}
              </CardTitle>
            </CardHeader>

            <CardContent className="space-y-4">
              <div className="space-y-1.5">
                <label className="text-base font-medium text-foreground">
                  {t("common:package_name")}
                </label>
                <Combobox
                  value={newFpsLockPackage}
                  onValueChange={setNewFpsLockPackage}
                  options={apps.map((app) => ({
                    label: app.package_name,
                    value: app.package_name,
                    disabled: Object.keys(fpsLock).includes(app.package_name),
                  }))}
                  placeholder={t("common:search_app")}
                  emptyText={
                    isLoading ? t("common:loading") : t("common:no_apps_found")
                  }
                  searchText={t("common:search_app")}
                  className="bg-card border-border"
                />
              </div>

              <div className="space-y-1.5">
                <label className="text-base font-medium text-foreground">
                  {t("common:fps_min_max")}
                </label>
                <div className="flex gap-2">
                  <Input
                    ref={minInputRef}
                    type="number"
                    value={newFpsLockMin}
                    onChange={(e) => setNewFpsLockMin(e.target.value)}
                    placeholder="Min"
                    className="h-10 bg-card border-border focus-visible:ring-offset-0 focus-visible:ring-primary"
                  />
                  <Input
                    type="number"
                    value={newFpsLockMax}
                    onChange={(e) => setNewFpsLockMax(e.target.value)}
                    placeholder="Max"
                    className="h-10 bg-card border-border focus-visible:ring-offset-0 focus-visible:ring-primary"
                  />
                </div>
              </div>
            </CardContent>

            <CardFooter className="flex justify-end gap-2 pt-2">
              <Button
                onClick={() => setIsAddingFpsLock(false)}
                variant="outline"
                className="border-border hover:bg-muted hover:text-foreground"
              >
                {t("common:cancel")}
              </Button>
              <Button
                onClick={addNewFpsLock}
                variant="default"
                className="bg-primary text-primary-foreground hover:bg-primary/90"
              >
                {t("common:add")}
              </Button>
            </CardFooter>
          </Card>
        </div>
      )}
    </div>
  );
}
